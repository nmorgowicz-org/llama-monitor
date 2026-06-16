// ── Settings ──────────────────────────────────────────────────────────────────
// Settings modal: collect, save, apply, dirty tracking, and event bindings.

import { settingsState } from '../core/app-state.js';
import { setContextCardViewPreference } from './context-card.js';
import { renderChatMessages } from './chat-render.js';
import { getAutoPollingInterval } from './network-detection.js';
import { showToast } from './toast.js';
import { setEnterToSend, applyChatStyle } from './chat-params.js';

// ── Secret masking helpers ────────────────────────────────────────────────────

function maskSecret(value) {
    if (!value || value.length <= 8) {
        return '•'.repeat(value?.length || 0);
    }
    const start = value.slice(0, 4);
    const end = value.slice(-4);
    const mid = '•'.repeat(8);
    return start + mid + end;
}

function applySecretValue(input, value, showRaw) {
    if (!input || value == null) return;
    const v = String(value);
    input.dataset.fullValue = v;
    if (showRaw) {
        input.value = v;
    } else {
        input.value = maskSecret(v);
    }
}

function getSecretValue(id) {
    const input = document.getElementById(id);
    if (!input) return '';
    const full = input.dataset.fullValue;
    if (full !== undefined && full !== '') {
        return full;
    }
    return (input.value || '').trim();
}

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

function notifySettingsApplied() {
    window.dispatchEvent(new CustomEvent('settings-applied', { detail: { ...settingsState } }));
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

    // T-058: read sleep_mode settings
    const sleepModeEl = document.getElementById('settings-sleep-mode-enabled');
    const sleepWhenHiddenEl = document.getElementById('settings-sleep-mode-when-hidden');
    const sleepIdleEl = document.getElementById('settings-sleep-mode-idle');

    const sleepModeEnabled = sleepModeEl ? (sleepModeEl.checked === true) : undefined;
    const sleepWhenHidden = sleepWhenHiddenEl ? (sleepWhenHiddenEl.checked === true) : undefined;
    const sleepIdleRaw = sleepIdleEl ? (sleepIdleEl.value || '1800') : '1800';
    const sleepIdleSecs = sleepIdleRaw === '0'
        ? null
        : (parseInt(sleepIdleRaw, 10) || 1800);

    return {
        preset_id: document.getElementById('preset-select').value,
        port: port,
        llama_server_path: document.getElementById('set-server-path').value,
        llama_server_cwd: document.getElementById('set-server-cwd').value,
        models_dir: document.getElementById('settings-models-dir')?.value.trim() || '',
        extra_models_dirs: _getExtraModelsDirs(),
        server_endpoint: endpoint,
        remote_agent_url: document.getElementById('set-remote-agent-url')?.value.trim() || '',
        remote_agent_token: getSecretValue('set-remote-agent-token') || '',
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
        chat_date_format: document.getElementById('settings-chat-date-format')?.value || settingsState.chat_date_format || 'MM/DD/YY',
        enter_to_send: document.getElementById('settings-enter-to-send') ? !!document.getElementById('settings-enter-to-send').checked : settingsState.enter_to_send !== false,
        context_notes_sidebar_expanded: !!settingsState.context_notes_sidebar_expanded,
        context_notes_intro_hidden: !!settingsState.context_notes_intro_hidden,
        persist_thinking_content: !!document.getElementById('settings-persist-thinking-content')?.checked,
        custom_suggestion_categories: settingsState.custom_suggestion_categories || {},
        // T-058: sleep_mode settings (sent to server via PUT /api/settings)
        sleep_mode: {
            auto_sleep_when_all_hidden: sleepWhenHidden !== undefined ? sleepWhenHidden : true,
            auto_sleep_idle_secs: sleepModeEnabled ? sleepIdleSecs : null,
        },
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
        (window.authFetch || fetch)('/api/settings', {
            method: 'PUT',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
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

    if (s.models_dir !== undefined) {
        const el = document.getElementById('settings-models-dir');
        if (el) el.value = s.models_dir;
        _updateModelsDirHint(s.models_dir);
    }

    if (Array.isArray(s.extra_models_dirs)) {
        _renderExtraModelsDirs(s.extra_models_dirs);
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
        if (el) applySecretValue(el, s.remote_agent_token, false);
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

    // Update central settingsState so feature modules read from a single source
    settingsState.enabled_context_notes = s.enabled_context_notes !== false;
    settingsState.enabled_suggestions = s.enabled_suggestions !== false;
    settingsState.enabled_quick_guide = s.enabled_quick_guide !== false;
    settingsState.suggestion_prompts = s.suggestion_prompts || {};
    settingsState.context_depth = s.context_depth || 10;
    settingsState.suggestion_count = s.suggestion_count || 5;
    settingsState.chat_date_format = s.chat_date_format || 'MM/DD/YY';
    settingsState.enter_to_send = s.enter_to_send !== false;
    settingsState.context_notes_sidebar_expanded = !!s.context_notes_sidebar_expanded;
    settingsState.context_notes_intro_hidden = !!s.context_notes_intro_hidden;
    settingsState.persist_thinking_content = !!s.persist_thinking_content;
    settingsState.custom_suggestion_categories = s.custom_suggestion_categories || {};

    const persistThinkingEl = document.getElementById('settings-persist-thinking-content');
    if (persistThinkingEl) persistThinkingEl.checked = settingsState.persist_thinking_content;

    const dateFmtEl = document.getElementById('chat-date-format');
    if (dateFmtEl) dateFmtEl.value = settingsState.chat_date_format;

    const settingsDateFmtEl = document.getElementById('settings-chat-date-format');
    if (settingsDateFmtEl) settingsDateFmtEl.value = settingsState.chat_date_format;

    const enterToSendEl = document.getElementById('settings-enter-to-send');
    if (enterToSendEl) enterToSendEl.checked = settingsState.enter_to_send !== false;

    const ctxView = s.context_card_view || 'gauge';
    document.querySelectorAll('.settings-ctx-view').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.view === ctxView);
    });

    notifySettingsApplied();
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
        headers: window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' },
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
    if (dateFmtEl) dateFmtEl.value = settingsState.chat_date_format || 'MM/DD/YY';

    const settingsDateFmtEl = document.getElementById('settings-chat-date-format');
    if (settingsDateFmtEl) settingsDateFmtEl.value = settingsState.chat_date_format || 'MM/DD/YY';

    const enterToSendEl = document.getElementById('settings-enter-to-send');
    if (enterToSendEl) enterToSendEl.checked = settingsState.enter_to_send !== false;

    const ctxView = settingsState.context_card_view || 'gauge';
    document.querySelectorAll('.settings-ctx-view').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.view === ctxView);
    });

    _initAppearanceTab();

    // Refresh live-changing fields from server
    (window.authFetch || fetch)('/api/settings', {
        headers: window.authHeaders ? window.authHeaders() : {},
    }).then(r => r.ok ? r.json() : null).then(s => { if (s) applySettings(s); }).catch(() => {});
    _refreshHfTokenStatus();
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

    // Date format — shared workspace preference
    document.getElementById('chat-date-format')?.addEventListener('change', (e) => {
        settingsState.chat_date_format = e.target.value;
        renderChatMessages();
        saveSettings();
    });

    // Auto-save on controls change
    const controls = document.getElementById('controls');
    if (controls) {
        controls.addEventListener('input', saveSettings);
        controls.addEventListener('change', saveSettings);
    }

    // Session tab controls
    document.getElementById('settings-chat-date-format')?.addEventListener('change', (e) => {
        settingsState.chat_date_format = e.target.value;
        const mirror = document.getElementById('chat-date-format');
        if (mirror) mirror.value = e.target.value;
        renderChatMessages();
        saveSettings();
    });

    document.getElementById('settings-enter-to-send')?.addEventListener('change', (e) => {
        setEnterToSend(e.target.checked);
        saveSettings();
    });

    document.querySelectorAll('.settings-ctx-view').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.settings-ctx-view').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            const view = btn.dataset.view;
            const mainBtn = document.getElementById('context-view-toggle-' + view);
            if (mainBtn) mainBtn.click();
            saveSettings();
        });
    });

    document.getElementById('settings-open-config-btn')?.addEventListener('click', () => {
        closeSettingsModal();
        setTimeout(() => document.getElementById('config-modal')?.classList.add('open'), 260);
    });

    document.getElementById('settings-advanced-open-config-btn')?.addEventListener('click', () => {
        closeSettingsModal();
        setTimeout(() => document.getElementById('config-modal')?.classList.add('open'), 260);
    });

    // GPU tab — load hardware info when tab opens (also load on first open)
    document.getElementById('settings-open-presets-btn')?.addEventListener('click', () => {
        closeSettingsModal();
        setTimeout(() => document.getElementById('preset-edit-btn')?.click(), 260);
    });

    // Models tab buttons
    document.getElementById('settings-open-models-btn')?.addEventListener('click', () => {
        closeSettingsModal();
        setTimeout(() => import('./models.js').then(({ openModelsModal }) => openModelsModal()), 260);
    });

    // Appearance tab live controls
    document.getElementById('settings-appearance-theme')?.addEventListener('change', (e) => {
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-chat-style')?.addEventListener('change', () => {
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-font-scale')?.addEventListener('input', (e) => {
        const el = document.getElementById('settings-appearance-font-scale-value');
        if (el) el.textContent = Number(e.target.value).toFixed(1) + '×';
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-spacing-scale')?.addEventListener('input', (e) => {
        const el = document.getElementById('settings-appearance-spacing-scale-value');
        if (el) el.textContent = Number(e.target.value).toFixed(1) + '×';
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-chat-font')?.addEventListener('input', (e) => {
        const el = document.getElementById('settings-appearance-chat-font-value');
        if (el) el.textContent = e.target.value + '%';
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-timestamps')?.addEventListener('change', () => {
        _applyAndSaveAppearance();
    });
    document.getElementById('settings-appearance-msg-width')?.addEventListener('change', () => {
        _applyAndSaveAppearance();
    });

    // Palette swatch picker — single delegated listener, bound once in initSettings
    document.getElementById('settings-palette-grid')?.addEventListener('click', (e) => {
        const btn = e.target.closest('.palette-swatch');
        if (!btn) return;
        const palette = btn.dataset.palette || '';
        document.querySelectorAll('.palette-swatch').forEach(b => {
            b.classList.toggle('active', b === btn);
            b.setAttribute('aria-pressed', b === btn ? 'true' : 'false');
        });
        _applyPalette(palette);
        _applyAndSaveAppearance();
    });

    // Settings tabs
    document.querySelectorAll('.settings-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            const target = tab.dataset.tab;
            document.querySelectorAll('.settings-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.settings-pane').forEach(p => p.classList.remove('active'));
            tab.classList.add('active');
            document.getElementById('settings-' + target)?.classList.add('active');

            if (target === 'security') {
                loadTlsConfig();
                loadDashboardAuthConfig();
            }
            if (target === 'gpu') {
                _loadSettingsGpuInfo();
            }
            if (target === 'appearance') {
                _initAppearanceTab();
            }
        });
    });

    // Certificate mode pills
    document.querySelectorAll('.cert-mode-pill').forEach(pill => {
        pill.addEventListener('click', () => {
            const mode = pill.dataset.mode;
            if (!mode) return;
            setActiveCertMode(mode);
        });
    });

    document.querySelectorAll('#dashboard-auth-mode-pills .cert-mode-pill').forEach(pill => {
        pill.addEventListener('click', () => {
            const mode = pill.dataset.authMode;
            if (!mode) return;
            setActiveDashboardAuthMode(mode);
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

    // Secret show/hide toggles
    document.querySelectorAll('.secret-toggle').forEach(btn => {
        btn.addEventListener('click', () => {
            const wrap = btn.parentElement;
            const input = wrap.querySelector('input');
            if (!input) return;
            const full = input.dataset.fullValue;
            if (!full) return;

            const isShowing = btn.dataset.showing === 'true';
            if (isShowing) {
                input.value = maskSecret(full);
                btn.dataset.showing = 'false';
            } else {
                input.value = full;
                btn.dataset.showing = 'true';
            }
        });
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

// ── GPU info (Settings > GPU tab) ────────────────────────────────────────────

async function _loadSettingsGpuInfo() {
    const el = document.getElementById('settings-gpu-info');
    if (!el) return;
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const res = await (window.authFetch || fetch)('/api/system/info', { headers });
        if (!res.ok) { el.textContent = 'Unable to load hardware info.'; return; }
        const d = await res.json();

        const wrap = document.createElement('div');
        const mut = document.createTreeWalker(wrap, NodeFilter.SHOW_ELEMENT);

        const makeRow = (label, value) => {
            const div = document.createElement('div');
            div.style.marginBottom = '6px';
            const span = document.createElement('span');
            span.style.color = 'var(--color-text-muted)';
            span.style.fontSize = '11px';
            span.textContent = label;
            div.appendChild(span);
            const br = document.createElement('br');
            div.appendChild(br);
            div.appendChild(document.createTextNode(value));
            wrap.appendChild(div);
        };

        if (d.cpu_name) {
            let extra = '';
            if (d.p_cores) {
                extra = ' \u00B7 ' + d.p_cores + 'P + ' + (d.e_cores || 0) + 'E cores';
            }
            makeRow('CPU', d.cpu_name + extra);
        }

        if (d.ram_total_gb) {
            makeRow('UNIFIED MEMORY', d.ram_total_gb.toFixed(1) + ' GB total');
        }

        if (Array.isArray(d.gpus) && d.gpus.length) {
            for (const g of d.gpus) {
                const vramMb = g.vram_total_mb || g.total_mb || g.total_memory_mb || g.vram_total || 0;
                const name = g.name || 'Unknown GPU';
                const vramPart = vramMb ? ' \u00B7 ' + (vramMb / 1024).toFixed(1) + ' GB VRAM' : '';
                makeRow('GPU', name + vramPart);
            }
        }

        if (wrap.firstChild) {
            while (wrap.firstChild) el.appendChild(wrap.firstChild);
        } else {
            el.textContent = 'No hardware info available.';
        }
    } catch {
        el.textContent = 'Unable to load hardware info.';
    }
}

// ── Appearance (Settings > Appearance tab) ───────────────────────────────────

function _applyPalette(palette) {
    const html = document.documentElement;
    html.classList.add('palette-changing');
    setTimeout(() => html.classList.remove('palette-changing'), 350);
    if (palette && palette !== 'carbon-mint') {
        html.dataset.palette = palette;
    } else {
        delete html.dataset.palette;
    }
}

function _initAppearanceTab() {
    const saved = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
    const themeEl = document.getElementById('settings-appearance-theme');
    const fontEl = document.getElementById('settings-appearance-font-scale');
    const fontValEl = document.getElementById('settings-appearance-font-scale-value');
    const spacingEl = document.getElementById('settings-appearance-spacing-scale');
    const spacingValEl = document.getElementById('settings-appearance-spacing-scale-value');
    const chatStyleEl = document.getElementById('settings-appearance-chat-style');
    const chatFontEl = document.getElementById('settings-appearance-chat-font');
    const chatFontValEl = document.getElementById('settings-appearance-chat-font-value');
    const timestampsEl = document.getElementById('settings-appearance-timestamps');
    const msgWidthEl = document.getElementById('settings-appearance-msg-width');

    // Restore palette swatch selection
    const activePalette = saved.palette || '';
    document.querySelectorAll('.palette-swatch').forEach(btn => {
        const matches = (btn.dataset.palette || '') === activePalette;
        btn.classList.toggle('active', matches);
        btn.setAttribute('aria-pressed', String(matches));
    });

    if (themeEl) themeEl.value = saved.theme || 'dark';
    if (fontEl) { fontEl.value = saved.fontScale || '1'; if (fontValEl) fontValEl.textContent = Number(saved.fontScale || 1).toFixed(1) + '×'; }
    if (spacingEl) { spacingEl.value = saved.spacingScale || '1'; if (spacingValEl) spacingValEl.textContent = Number(saved.spacingScale || 1).toFixed(1) + '×'; }
    if (chatStyleEl) chatStyleEl.value = localStorage.getItem('llama-monitor-chat-style') || 'rounded';
    const savedChatFont = parseInt(localStorage.getItem('llama-monitor-chat-font') || '100');
    if (chatFontEl) { chatFontEl.value = savedChatFont; if (chatFontValEl) chatFontValEl.textContent = savedChatFont + '%'; }
    if (timestampsEl) timestampsEl.value = saved.timestamps || 'hover';
    if (msgWidthEl) msgWidthEl.value = saved.msgWidth || 'normal';
}

function _applyAndSaveAppearance() {
    const theme = document.getElementById('settings-appearance-theme')?.value || 'dark';
    const fontScale = document.getElementById('settings-appearance-font-scale')?.value || '1';
    const spacingScale = document.getElementById('settings-appearance-spacing-scale')?.value || '1';
    const chatStyle = document.getElementById('settings-appearance-chat-style')?.value || 'rounded';
    const chatFont = parseInt(document.getElementById('settings-appearance-chat-font')?.value || '100');
    const timestamps = document.getElementById('settings-appearance-timestamps')?.value || 'hover';
    const msgWidth = document.getElementById('settings-appearance-msg-width')?.value || 'normal';
    const palette = document.querySelector('#settings-palette-grid .palette-swatch.active')?.dataset.palette || '';

    const effectiveTheme = theme === 'auto'
        ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
        : theme;
    document.documentElement.dataset.theme = effectiveTheme;
    document.documentElement.style.fontSize = (Number(fontScale) * 16) + 'px';
    document.documentElement.style.setProperty('--gap-md', (Number(spacingScale) * 16) + 'px');
    applyChatStyle(chatStyle);

    // Chat font size — mirrors what chat-params.js does
    const chatFontScale = chatFont / 100;
    const chatMessages = document.getElementById('chat-messages');
    if (chatMessages) chatMessages.style.setProperty('--chat-font-scale', chatFontScale);
    const chatInputRow = document.getElementById('chat-input-row');
    if (chatInputRow) chatInputRow.style.setProperty('--chat-font-scale', chatFontScale);

    // Timestamps visibility
    const chatPage = document.querySelector('.chat-page');
    if (chatPage) {
        if (timestamps === 'hover') delete chatPage.dataset.timestamps;
        else chatPage.dataset.timestamps = timestamps;
    }

    // Message max-width — set on the messages container so child .chat-message inherits
    const widthMap = { narrow: '65%', normal: '82%', wide: '100%' };
    const chatMsgsEl = document.getElementById('chat-messages');
    if (chatMsgsEl) chatMsgsEl.style.setProperty('--chat-message-max-width', widthMap[msgWidth] || '82%');

    localStorage.setItem('llama-monitor-chat-style', chatStyle);
    localStorage.setItem('llama-monitor-chat-font', chatFont);
    localStorage.setItem('llama-monitor-preferences', JSON.stringify({ theme, palette, fontScale, spacingScale, timestamps, msgWidth }));

    // Sync pref-* elements in user-preferences-modal so they stay consistent
    const prefTheme = document.getElementById('pref-theme-mode');
    if (prefTheme) prefTheme.value = theme;
    const prefFont = document.getElementById('pref-font-scale');
    if (prefFont) prefFont.value = fontScale;
    const prefSpacing = document.getElementById('pref-spacing-scale');
    if (prefSpacing) prefSpacing.value = spacingScale;
    const prefChatStyle = document.getElementById('pref-chat-style');
    if (prefChatStyle) prefChatStyle.value = chatStyle;
}

// ── TLS / Certificates ────────────────────────────────────────────────────────

export function setActiveCertMode(mode) {
    const pills = document.querySelectorAll('.cert-mode-pill');
    const panes = document.querySelectorAll('.cert-mode-content');

    pills.forEach(p => {
        const m = p.dataset.mode;
        if (m === mode) {
            p.classList.add('active');
        } else {
            p.classList.remove('active');
        }
    });

    panes.forEach(p => {
        const id = p.id;
        const show = id === 'cert-mode-' + mode;
        p.style.display = show ? 'block' : 'none';
    });
}

async function loadTlsConfig() {
    const statusEl = document.getElementById('tls-status-text');
    const detailsEl = document.getElementById('tls-details');
    const warningEl = document.getElementById('tls-lan-warning');

    if (!statusEl) return;

    try {
        const res = await fetch('/api/tls/config', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });

        if (!res.ok) {
            statusEl.textContent = 'TLS: Unable to check status';
            if (detailsEl) detailsEl.textContent = `Server responded ${res.status}`;
            return;
        }

        const data = await res.json();
        const mode = data?.mode || 'none';
        const host = data?.host || '';

        // Set active pill and show corresponding pane
        setActiveCertMode(mode);

        // Update status text
        if (mode === 'none') {
            statusEl.textContent = 'TLS: Disabled (HTTP only)';
        } else if (mode === 'self-signed') {
            statusEl.textContent = 'TLS: Enabled (Self-signed)';
        } else if (mode === 'custom') {
            statusEl.textContent = 'TLS: Enabled (Custom certificate)';
        } else if (mode === 'acme') {
            const env = (data?.acme?.environment || '').toLowerCase();
            if (env === 'staging') {
                statusEl.textContent = 'TLS: Enabled (Let\'s Encrypt – Staging)';
            } else {
                statusEl.textContent = 'TLS: Enabled (Let\'s Encrypt – Production)';
            }
        } else {
            statusEl.textContent = 'TLS: Enabled';
        }

        // Future: show cert details when backend provides them
        if (detailsEl && (data?.issuer || data?.expiry || data?.domains)) {
            const parts = [];
            if (data.issuer) parts.push('Issuer: ' + data.issuer);
            if (data.expiry) parts.push('Expires: ' + data.expiry);
            if (data.domains && data.domains.length) parts.push('Domains: ' + data.domains.join(', '));
            detailsEl.textContent = parts.join(' · ');
        } else if (detailsEl) {
            detailsEl.textContent = '';
        }

        // Show LAN warning if 0.0.0.0 and no TLS
        if (warningEl) {
            if (mode === 'none' && host === '0.0.0.0') {
                warningEl.style.display = 'block';
            } else {
                warningEl.style.display = 'none';
            }
        }

        // Pre-fill ACME fields when mode is "acme"
        if (mode === 'acme' && data?.acme) {
            const acme = data.acme;
            const fqdnEl = document.getElementById('acme-fqdn');
            const emailEl = document.getElementById('acme-email');
            const providerEl = document.getElementById('acme-dns-provider');
            const customWrapEl = document.getElementById('acme-provider-custom-wrap');
            const customEl = document.getElementById('acme-dns-provider-custom');
            const stagingRadio = document.getElementById('acme-env-staging');
            const prodRadio = document.getElementById('acme-env-production');
            const delayEl = document.getElementById('acme-validation-delay');

            if (fqdnEl) fqdnEl.value = acme.fqdn || '';
            if (emailEl) emailEl.value = acme.email || '';

            const prov = (acme.dns_provider || '').toLowerCase();
            if (providerEl) {
                // If provider is in known list, select it; otherwise select __other__
                const knownProviders = [
                    'cloudflare', 'route53', 'gcloud', 'digitalocean',
                    'namecheap', 'porkbun', 'godaddy', 'azure-dns',
                    'hetzner', 'ovh', 'dnsmadeeasy', 'powerdns',
                    'duckdns', 'inwx'
                ];
                if (knownProviders.includes(prov)) {
                    providerEl.value = prov;
                    if (customWrapEl) customWrapEl.style.display = 'none';
                } else {
                    providerEl.value = '__other__';
                    if (customWrapEl) customWrapEl.style.display = 'block';
                    if (customEl) customEl.value = acme.dns_provider || '';
                }
            }

            const env = (acme.environment || 'staging').toLowerCase();
            if (stagingRadio) stagingRadio.checked = env === 'staging';
            if (prodRadio) prodRadio.checked = env === 'production';

            if (delayEl) delayEl.value = acme.validation_delay ?? 300;

            // Populate key/value credentials from dnsConfig
            clearAcmeCredentials();
            const dnsCfg = acme.dns_config || {};
            for (const [k, v] of Object.entries(dnsCfg)) {
                addAcmeCredentialRow(k, v);
            }

            // Show last renewal in tls-details if present
            if (acme.last_renewal && detailsEl) {
                detailsEl.textContent =
                    (detailsEl.textContent ? detailsEl.textContent + ' · ' : '') +
                    'Last renewal: ' + acme.last_renewal;
            }
        }
    } catch (err) {
        statusEl.textContent = 'TLS: Unable to check status';
        if (detailsEl) detailsEl.textContent = 'Network or server error';
        console.warn('[settings] TLS config load failed:', err);
    }
}

async function tlsPut(payload) {
    try {
        const res = await fetch('/api/tls/config', {
            method: 'PUT',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });

        if (!res.ok) {
            const text = await res.text().catch(() => '');
            showToast('TLS update failed', 'error', text || `Server responded ${res.status}`);
            return;
        }

        return await res.json().catch(() => ({}));
    } catch (err) {
        showToast('TLS update failed', 'error', err.message || 'Network error');
    }
}

function _bindTlsEvents() {
    // Disable TLS
    const disableBtn = document.getElementById('btn-disable-tls');
    if (disableBtn) {
        disableBtn.addEventListener('click', async () => {
            await tlsPut({ mode: 'none' });
            showToast('TLS disabled', 'success', 'Restart llama-monitor to apply.');
            await loadTlsConfig();
        });
    }

    // Generate self-signed
    const selfSignedBtn = document.getElementById('btn-generate-self-signed');
    if (selfSignedBtn) {
        selfSignedBtn.addEventListener('click', async () => {
            await tlsPut({ mode: 'self-signed' });
            showToast('Self-signed TLS enabled', 'success', 'Restart llama-monitor to apply.');
            await loadTlsConfig();
        });
    }

    // Apply custom certificate
    const applyCustomBtn = document.getElementById('btn-apply-custom-cert');
    if (applyCustomBtn) {
        applyCustomBtn.addEventListener('click', async () => {
            const certPath = (document.getElementById('tls-custom-cert-path')?.value || '').trim();
            const keyPath = (document.getElementById('tls-custom-key-path')?.value || '').trim();

            if (!certPath || !keyPath) {
                showToast('Missing paths', 'error', 'Both certificate and key paths are required.');
                return;
            }

            await tlsPut({
                mode: 'custom',
                custom_cert_path: certPath,
                custom_key_path: keyPath,
            });
            showToast('Custom certificate configured', 'success', 'Restart llama-monitor to apply.');
            await loadTlsConfig();
        });
    }

    // ACME: show/hide custom provider input
    const providerSelect = document.getElementById('acme-dns-provider');
    if (providerSelect) {
        providerSelect.addEventListener('change', () => {
            const customWrap = document.getElementById('acme-provider-custom-wrap');
            if (customWrap) {
                customWrap.style.display = providerSelect.value === '__other__' ? 'block' : 'none';
            }
        });
    }

    // ACME: add credential row
    const addCredBtn = document.getElementById('acme-add-credential');
    if (addCredBtn) {
        addCredBtn.addEventListener('click', () => {
            addAcmeCredentialRow('', '');
        });
    }

    // ACME: Request certificate
    const acmeRequestBtn = document.getElementById('acme-request-cert');
    if (acmeRequestBtn) {
        acmeRequestBtn.addEventListener('click', async () => {
            const statusEl = document.getElementById('acme-status-text');

            const fqdn = (document.getElementById('acme-fqdn')?.value || '').trim();
            const email = (document.getElementById('acme-email')?.value || '').trim();
            const providerValue = providerSelect?.value || 'cloudflare';
            const customProvider = (document.getElementById('acme-dns-provider-custom')?.value || '').trim();
            const provider = providerValue === '__other__' ? customProvider : providerValue;
            const env = (document.querySelector('input[name="acme-env"]:checked')?.value || 'staging');
            const delay = parseInt(document.getElementById('acme-validation-delay')?.value || '300', 10);

            const dnsConfig = readAcmeCredentials();

            // Basic validation
            if (!fqdn) {
                showToast('Missing domain', 'error', 'Enter a domain (FQDN) for your certificate.');
                return;
            }
            if (!provider || provider === '__other__') {
                showToast('Missing provider', 'error', 'Select or type a DNS provider.');
                return;
            }
            if (Object.keys(dnsConfig).length === 0) {
                showToast('Missing credentials', 'error', 'Add at least one credential key/value for your DNS provider.');
                return;
            }

            const payload = {
                mode: 'acme',
                acme: {
                    enabled: true,
                    fqdn,
                    email,
                    environment: env,
                    dns_provider: provider,
                    dns_config: dnsConfig,
                    validation_delay: delay,
                },
            };

            if (statusEl) statusEl.textContent = 'Saving ACME configuration...';

            // Save config
            const putResult = await tlsPut(payload);
            if (!putResult) {
                if (statusEl) statusEl.textContent = 'Failed to save ACME configuration.';
                return;
            }

            if (statusEl) statusEl.textContent = 'Requesting certificate...';

            // Trigger ACME request
            try {
                const res = await fetch('/api/tls/acme/request', {
                    method: 'POST',
                    headers: window.authHeaders
                        ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                        : { 'Content-Type': 'application/json' },
                });

                if (!res.ok) {
                    const text = await res.text().catch(() => '');
                    if (statusEl) statusEl.textContent = 'Request failed: ' + (text || `Server responded ${res.status}`);
                    return;
                }

                if (statusEl) {
                    statusEl.textContent = 'Certificate requested. Restart llama-monitor to apply.';
                }
                showToast('ACME certificate requested', 'success', 'Restart llama-monitor to apply.');
                await loadTlsConfig();
            } catch (err) {
                if (statusEl) statusEl.textContent = 'Request failed: ' + (err.message || 'Network error');
            }
        });
    }

    // ACME: Renew certificate
    const acmeRenewBtn = document.getElementById('acme-renew-cert');
    if (acmeRenewBtn) {
        acmeRenewBtn.addEventListener('click', async () => {
            const statusEl = document.getElementById('acme-status-text');
            if (statusEl) statusEl.textContent = 'Renewing certificate...';

            try {
                const res = await fetch('/api/tls/acme/renew', {
                    method: 'POST',
                    headers: window.authHeaders
                        ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                        : { 'Content-Type': 'application/json' },
                });

                if (!res.ok) {
                    const text = await res.text().catch(() => '');
                    if (statusEl) statusEl.textContent = 'Renewal failed: ' + (text || `Server responded ${res.status}`);
                    showToast('ACME renewal failed', 'error', text || `Server responded ${res.status}`);
                    return;
                }

                if (statusEl) {
                    statusEl.textContent = 'Certificate renewed. Restart llama-monitor to apply.';
                }
                showToast('ACME certificate renewed', 'success', 'Restart llama-monitor to apply.');
                await loadTlsConfig();
            } catch (err) {
                if (statusEl) statusEl.textContent = 'Renewal failed: ' + (err.message || 'Network error');
                showToast('ACME renewal failed', 'error', err.message || 'Network error');
            }
        });
    }
}

 // ── ACME credential helpers ──────────────────────────────────────────────────

function acmeCredentialsGrid() {
    return document.getElementById('acme-credentials-grid');
}

function addAcmeCredentialRow(key, value) {
    const grid = acmeCredentialsGrid();
    if (!grid) return;

    const keyInput = document.createElement('input');
    keyInput.type = 'text';
    keyInput.placeholder = 'Key (e.g. CLOUDFLARE_API_TOKEN)';
    keyInput.style.fontSize = '11px';
    keyInput.value = key || '';

    const valInput = document.createElement('input');
    valInput.type = 'password';
    valInput.placeholder = 'Value';
    valInput.style.fontSize = '11px';
    valInput.value = value || '';

    const removeBtn = document.createElement('button');
    removeBtn.type = 'button';
    removeBtn.textContent = '✕';
    removeBtn.style.fontSize = '10px';
    removeBtn.style.padding = '1px 5px';
    removeBtn.style.cursor = 'pointer';
    removeBtn.addEventListener('click', () => {
        keyInput.remove();
        valInput.remove();
        removeBtn.remove();
    });

    grid.appendChild(keyInput);
    grid.appendChild(valInput);
    grid.appendChild(removeBtn);
}

function clearAcmeCredentials() {
    const grid = acmeCredentialsGrid();
    if (!grid) return;
    // Keep header cells (first 3 children), remove the rest
    while (grid.children.length > 3) {
        grid.removeChild(grid.lastChild);
    }
}

function readAcmeCredentials() {
    const grid = acmeCredentialsGrid();
    if (!grid) return {};

    const inputs = Array.from(grid.querySelectorAll('input'));
    const map = {};
    for (let i = 0; i + 1 < inputs.length; i += 2) {
        const k = (inputs[i]?.value || '').trim();
        const v = (inputs[i + 1]?.value || '').trim();
        if (k && v) {
            map[k] = v;
        }
    }
    return map;
}

// ── Dashboard auth / password reset ─────────────────────────────────────────

function selectedDashboardAuthMode() {
    return document.querySelector('#dashboard-auth-mode-pills .cert-mode-pill.active')?.dataset.authMode || 'none';
}

function setActiveDashboardAuthMode(mode) {
    document.querySelectorAll('#dashboard-auth-mode-pills .cert-mode-pill').forEach(pill => {
        pill.classList.toggle('active', pill.dataset.authMode === mode);
    });
}

async function loadDashboardAuthConfig() {
    const statusEl = document.getElementById('dashboard-auth-status');
    const warningEl = document.getElementById('dashboard-auth-managed-warning');
    const controlsEl = document.getElementById('dashboard-auth-controls');
    const userEl = document.getElementById('dashboard-auth-username');

    if (statusEl) statusEl.textContent = 'Loading dashboard access…';

    try {
        const res = await fetch('/api/auth/config', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const data = await res.json().catch(() => ({}));
        if (!res.ok) {
            if (statusEl) statusEl.textContent = data.message || data.error || `Failed to load dashboard access (${res.status})`;
            return;
        }

        if (warningEl) warningEl.style.display = data.managedByCli ? '' : 'none';
        if (controlsEl) {
            controlsEl.style.opacity = data.managedByCli ? '0.6' : '1';
            controlsEl.style.pointerEvents = data.managedByCli ? 'none' : 'auto';
        }

        if (userEl) userEl.value = data.username || '';

        if (data.basicEnabled && data.formEnabled) {
            setActiveDashboardAuthMode('both');
        } else if (data.basicEnabled) {
            setActiveDashboardAuthMode('basic');
        } else if (data.formEnabled) {
            setActiveDashboardAuthMode('form');
        } else {
            setActiveDashboardAuthMode('none');
        }

        if (statusEl) {
            const sourceLabel = data.managedByCli ? 'startup flags' : 'auth-config.json';
            const modeLabel = data.basicEnabled && data.formEnabled
                ? 'Basic Auth + form login'
                : data.basicEnabled
                    ? 'Basic Auth'
                    : data.formEnabled
                        ? 'Form login'
                        : 'No dashboard auth';
            statusEl.textContent = `${modeLabel} • managed via ${sourceLabel}`;
        }
    } catch (err) {
        if (statusEl) statusEl.textContent = err.message || 'Failed to load dashboard access';
    }
}

async function saveDashboardAuthConfig() {
    const statusEl = document.getElementById('dashboard-auth-save-status');
    const username = document.getElementById('dashboard-auth-username')?.value.trim() || '';
    const currentPassword = document.getElementById('dashboard-auth-current-password')?.value || '';
    const newPassword = document.getElementById('dashboard-auth-new-password')?.value || '';
    const confirmPassword = document.getElementById('dashboard-auth-confirm-password')?.value || '';
    const mode = selectedDashboardAuthMode();

    const basicEnabled = mode === 'basic' || mode === 'both';
    const formEnabled = mode === 'form' || mode === 'both';

    if (newPassword || confirmPassword) {
        if (newPassword !== confirmPassword) {
            if (statusEl) statusEl.textContent = 'Passwords do not match';
            showToast('Dashboard access not saved', 'error', 'New password and confirmation must match.');
            return;
        }
    }

    if ((basicEnabled || formEnabled) && !username) {
        if (statusEl) statusEl.textContent = 'Username required';
        showToast('Dashboard access not saved', 'error', 'Enter a username when auth is enabled.');
        return;
    }

    if (statusEl) statusEl.textContent = 'Saving…';

    try {
        const res = await fetch('/api/auth/config', {
            method: 'PUT',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                basic_enabled: basicEnabled,
                form_enabled: formEnabled,
                username,
                current_password: currentPassword,
                new_password: newPassword,
            }),
        });
        const data = await res.json().catch(() => ({}));
        if (!res.ok) {
            if (statusEl) statusEl.textContent = data.message || data.error || `Failed (${res.status})`;
            showToast('Dashboard access not saved', 'error', data.message || data.error || `Server responded ${res.status}`);
            return;
        }

        if (statusEl) statusEl.textContent = data.message || 'Saved';
        showToast('Dashboard access saved', 'success', data.message || 'Dashboard access updated.');

        ['dashboard-auth-current-password', 'dashboard-auth-new-password', 'dashboard-auth-confirm-password'].forEach(id => {
            const el = document.getElementById(id);
            if (el) el.value = '';
        });

        await loadDashboardAuthConfig();
    } catch (err) {
        if (statusEl) statusEl.textContent = err.message || 'Network error';
        showToast('Dashboard access not saved', 'error', err.message || 'Network error');
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSettings() {
    // Bind settings button (sidebar button also bound by nav.js with data-tab="settings")
    document.getElementById('settings-btn')?.addEventListener('click', openSettingsModal);
    document.getElementById('btn-save-dashboard-auth')?.addEventListener('click', saveDashboardAuthConfig);

    // Bind settings modal buttons
    document.getElementById('settings-modal-close')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-cancel')?.addEventListener('click', closeSettingsModal);
    document.getElementById('settings-modal-save')?.addEventListener('click', saveSettings);

    // Rotate Agent Token
    document.getElementById('btn-rotate-agent-token')?.addEventListener('click', async () => {
        const statusEl = document.getElementById('rotate-agent-token-status');

        if (!await confirmTokenRotation(
            'Rotate Agent Token?',
            'This will invalidate the current remote agent token immediately.'
        )) {
            return;
        }

        if (statusEl) statusEl.textContent = 'Rotating...';

        try {
            const res = await fetch('/api/rotate-agent-token', {
                method: 'POST',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
            });

            if (!res.ok) {
                const text = await res.text().catch(() => '');
                if (statusEl) statusEl.textContent = 'Failed: ' + (text || `Server responded ${res.status}`);
                showToast('Rotate agent token failed', 'error', text || `Server responded ${res.status}`);
                return;
            }

            if (statusEl) statusEl.textContent = 'Token rotated';
            showToast('Agent token rotated', 'success', 'Previous token is now invalid.');

            // Refresh settings so masked token updates
            const settingsRes = await fetch('/api/settings', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (settingsRes.ok) {
                const s = await settingsRes.json();
                applySettings(s);
            }
        } catch (err) {
            if (statusEl) statusEl.textContent = 'Failed: ' + (err.message || 'Network error');
            showToast('Rotate agent token failed', 'error', err.message || 'Network error');
        }
    });

    // Rotate API Token
    document.getElementById('btn-rotate-api-token')?.addEventListener('click', async () => {
        const statusEl = document.getElementById('rotate-api-token-status');

        if (!await confirmTokenRotation(
            'Rotate API Token?',
            'This will invalidate the current API token immediately. Open browser tabs may lose access until refreshed.'
        )) {
            return;
        }

        if (statusEl) statusEl.textContent = 'Rotating...';

        try {
            const res = await fetch('/api/rotate-api-token', {
                method: 'POST',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
            });

            if (!res.ok) {
                const text = await res.text().catch(() => '');
                if (statusEl) statusEl.textContent = 'Failed: ' + (text || `Server responded ${res.status}`);
                showToast('Rotate API token failed', 'error', text || `Server responded ${res.status}`);
                return;
            }

            if (statusEl) statusEl.textContent = 'Token rotated';
            showToast('API token rotated', 'success', 'Previous token is now invalid. Restart llama-monitor to fully apply.');
        } catch (err) {
            if (statusEl) statusEl.textContent = 'Failed: ' + (err.message || 'Network error');
            showToast('Rotate API token failed', 'error', err.message || 'Network error');
        }
    });

    // Rotate DB Admin Token
    document.getElementById('btn-rotate-db-admin-token')?.addEventListener('click', async () => {
        const statusEl = document.getElementById('rotate-db-admin-token-status');

        if (!await confirmTokenRotation(
            'Rotate DB Admin Token?',
            'This will invalidate the current DB admin token immediately.'
        )) {
            return;
        }

        if (statusEl) statusEl.textContent = 'Rotating...';

        try {
            const res = await fetch('/api/rotate-db-admin-token', {
                method: 'POST',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
            });

            if (!res.ok) {
                const text = await res.text().catch(() => '');
                if (statusEl) statusEl.textContent = 'Failed: ' + (text || `Server responded ${res.status}`);
                showToast('Rotate DB admin token failed', 'error', text || `Server responded ${res.status}`);
                return;
            }

            if (statusEl) statusEl.textContent = 'Token rotated';
            showToast('DB admin token rotated', 'success', 'Previous token is now invalid. Restart llama-monitor to fully apply.');
        } catch (err) {
            if (statusEl) statusEl.textContent = 'Failed: ' + (err.message || 'Network error');
            showToast('Rotate DB admin token failed', 'error', err.message || 'Network error');
        }
    });

    _bindSettingsEvents();
    _bindTlsEvents();
    if (document.getElementById('settings-security')?.classList.contains('active')) {
        loadDashboardAuthConfig();
    }
    _bindModelSettingsEvents();
}

// ── Models directory + HF token helpers ──────────────────────────────────────

function _getExtraModelsDirs() {
    return [...document.querySelectorAll('#settings-extra-dirs-list .extra-dir-entry')]
        .map(el => el.dataset.path)
        .filter(Boolean);
}

function _renderExtraModelsDirs(dirs) {
    const list = document.getElementById('settings-extra-dirs-list');
    if (!list) return;
    list.innerHTML = '';
    for (const dir of dirs) _addExtraDirItem(dir);
}

function _addExtraDirItem(path) {
    if (!path) return;
    const list = document.getElementById('settings-extra-dirs-list');
    if (!list) return;
    // Deduplicate
    if ([...list.querySelectorAll('.extra-dir-entry')].some(el => el.dataset.path === path)) return;
    const item = document.createElement('div');
    item.className = 'extra-dir-entry';
    item.dataset.path = path;
    const label = document.createElement('span');
    label.className = 'extra-dir-path';
    label.textContent = path;
    label.title = path;
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn btn-ghost extra-dir-remove';
    btn.textContent = '×';
    btn.setAttribute('aria-label', 'Remove');
    btn.addEventListener('click', () => { item.remove(); markSettingsDirty(); });
    item.appendChild(label);
    item.appendChild(btn);
    list.appendChild(item);
}

function _updateModelsDirHint(dir) {
    const hint = document.getElementById('settings-models-dir-hint');
    if (!hint) return;
    if (dir) {
        hint.textContent = `Scanning: ${dir}`;
        hint.style.display = '';
    } else {
        hint.style.display = 'none';
    }
}

function _bindModelSettingsEvents() {
    // Models dir — browse button
    document.getElementById('settings-models-dir-browse')?.addEventListener('click', async () => {
        const { openDeferredFileBrowser } = await import('./file-browser-launcher.js');
        openDeferredFileBrowser('settings-models-dir', 'dir');
    });

    // Models dir — clear button
    document.getElementById('settings-models-dir-clear')?.addEventListener('click', () => {
        const el = document.getElementById('settings-models-dir');
        if (el) el.value = '';
        _updateModelsDirHint('');
        markSettingsDirty();
    });

    // Models dir — save on change
    document.getElementById('settings-models-dir')?.addEventListener('change', () => {
        const val = document.getElementById('settings-models-dir')?.value.trim() || '';
        _updateModelsDirHint(val);
        markSettingsDirty();
    });

    // Extra model dirs — add button
    document.getElementById('settings-extra-dir-add')?.addEventListener('click', () => {
        const input = document.getElementById('settings-extra-dir-input');
        const val = input?.value.trim();
        if (!val) return;
        _addExtraDirItem(val);
        if (input) input.value = '';
        markSettingsDirty();
    });

    // Extra model dirs — add on Enter in input
    document.getElementById('settings-extra-dir-input')?.addEventListener('keydown', e => {
        if (e.key === 'Enter') {
            e.preventDefault();
            document.getElementById('settings-extra-dir-add')?.click();
        }
    });

    // Extra model dirs — browse button
    document.getElementById('settings-extra-dir-browse')?.addEventListener('click', async () => {
        const { openDeferredFileBrowser } = await import('./file-browser-launcher.js');
        openDeferredFileBrowser('settings-extra-dir-input', 'dir');
    });

    // HF token — show/hide toggle
    document.getElementById('settings-hf-token-show')?.addEventListener('click', (e) => {
        const input = document.getElementById('settings-hf-token');
        if (!input) return;
        const isPassword = input.type === 'password';
        input.type = isPassword ? 'text' : 'password';
        e.currentTarget.textContent = isPassword ? 'Hide' : 'Show';
    });

    // HF token — save button
    document.getElementById('settings-hf-token-save')?.addEventListener('click', async () => {
        const token = document.getElementById('settings-hf-token')?.value.trim() || '';
        if (!token) return;
        const btn = document.getElementById('settings-hf-token-save');
        const origText = btn?.textContent;
        if (btn) { btn.disabled = true; btn.textContent = 'Saving…'; }
        try {
            const res = await (window.authFetch || fetch)('/api/hf/token', {
                method: 'PUT',
                headers: window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' },
                body: JSON.stringify({ token }),
            });
            const data = await res.json().catch(() => ({}));
            if (data.ok) {
                if (btn) { btn.textContent = '✓ Saved'; setTimeout(() => { btn.textContent = origText; btn.disabled = false; }, 1500); }
                _refreshHfTokenStatus();
                // Clear field after save
                const inputEl = document.getElementById('settings-hf-token');
                if (inputEl) inputEl.value = '';
            } else {
                if (btn) { btn.textContent = 'Failed'; setTimeout(() => { btn.textContent = origText; btn.disabled = false; }, 2000); }
            }
        } catch {
            if (btn) { btn.textContent = 'Error'; setTimeout(() => { btn.textContent = origText; btn.disabled = false; }, 2000); }
        }
    });

    // HF token — remove button
    document.getElementById('settings-hf-token-remove')?.addEventListener('click', async () => {
        try {
            await (window.authFetch || fetch)('/api/hf/token', {
                method: 'DELETE',
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            _refreshHfTokenStatus();
        } catch { /* ignore */ }
    });

    // Load HF token status when Models tab is activated
    document.querySelectorAll('.settings-tab[data-tab="models"]').forEach(btn => {
        btn.addEventListener('click', _refreshHfTokenStatus);
    });

    // Also load immediately if Models tab is already active
    if (document.getElementById('settings-models')?.classList.contains('active')) {
        _refreshHfTokenStatus();
    }
}

async function _refreshHfTokenStatus() {
    const statusEl = document.getElementById('settings-hf-token-status');
    const removeBtn = document.getElementById('settings-hf-token-remove');
    if (!statusEl) return;
    try {
        const res = await (window.authFetch || fetch)('/api/hf/token', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        if (!res.ok) return;
        const data = await res.json();
        statusEl.style.display = '';
        if (data.set) {
            statusEl.textContent = '✓ Token saved — authenticated HF requests active.';
            statusEl.style.color = 'var(--accent-green,#a3e635)';
            if (removeBtn) removeBtn.style.display = '';
        } else {
            statusEl.textContent = 'No token saved — rate limits apply to HF searches.';
            statusEl.style.color = 'var(--color-text-muted)';
            if (removeBtn) removeBtn.style.display = 'none';
        }
    } catch { /* ignore */ }
}

export { _refreshHfTokenStatus as refreshHfTokenStatus };

// ── Token rotation confirmation helper ────────────────────────────────────────

async function confirmTokenRotation(title, message) {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.zIndex = '2000';

    const dialog = document.createElement('div');
    dialog.className = 'modal';
    dialog.style.width = '420px';
    dialog.style.padding = '14px 16px 14px 16px';

    const header = document.createElement('div');
    header.style.display = 'flex';
    header.style.alignItems = 'center';
    header.style.justifyContent = 'space-between';
    header.style.marginBottom = '8px';

    const titleEl = document.createElement('div');
    titleEl.style.fontSize = '15px';
    titleEl.style.fontWeight = '600';
    titleEl.textContent = title;

    const msg = document.createElement('div');
    msg.style.fontSize = '13px';
    msg.style.color = 'var(--color-text-muted)';
    msg.style.marginBottom = '12px';
    msg.textContent = message;

    const actions = document.createElement('div');
    actions.style.display = 'flex';
    actions.style.justifyContent = 'flex-end';
    actions.style.gap = '8px';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'btn btn-modal-cancel';
    cancelBtn.textContent = 'Cancel';

    const confirmBtn = document.createElement('button');
    confirmBtn.type = 'button';
    confirmBtn.className = 'btn btn-modal-save';
    confirmBtn.textContent = 'Rotate';

    const result = new Promise(resolve => {
        let decided = false;

        function cleanup() {
            if (overlay.parentElement) overlay.remove();
        }

        cancelBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(false);
        });

        confirmBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(true);
        });

        overlay.addEventListener('click', (e) => {
            if (e.target === overlay && !decided) {
                decided = true;
                cleanup();
                resolve(false);
            }
        });
    });

    header.appendChild(titleEl);
    dialog.appendChild(header);
    dialog.appendChild(msg);
    dialog.appendChild(actions);
    actions.appendChild(cancelBtn);
    actions.appendChild(confirmBtn);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
    confirmBtn.focus();

    return result;
}
